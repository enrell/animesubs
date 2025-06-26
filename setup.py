#!/usr/bin/env python3
"""
Script para executar o tradutor de legendas com configurações básicas.
"""

import os
import sys
from pathlib import Path


def setup_environment():
    """Configura o ambiente e verifica dependências."""
    print("🔧 Configurando ambiente...")

    # Verifica se o arquivo .env existe
    env_file = Path(".env")
    if not env_file.exists():
        print("⚠️  Arquivo .env não encontrado!")
        print("📝 Criando arquivo .env de exemplo...")

        with open(".env", "w") as f:
            f.write("# Configure sua API key do Google Gemini aqui\n")
            f.write("GEMINI_API_KEY=your_api_key_here\n")
            f.write("TARGET_LANGUAGE=pt-BR\n")
            f.write("BATCH_SIZE=50\n")

        print("✅ Arquivo .env criado!")
        print("🔑 Edite o arquivo .env e adicione sua GEMINI_API_KEY")
        print("📋 Obtenha sua API key em: https://makersuite.google.com/app/apikey")
        return False

    print("✅ Arquivo .env encontrado!")
    return True


def show_usage_examples():
    """Mostra exemplos de uso do script."""
    print("\n📚 EXEMPLOS DE USO:")
    print("=" * 40)

    print("\n1️⃣  Tradução básica:")
    print("   python main.py exemplo.ass")

    print("\n2️⃣  Especificar arquivo de saída:")
    print("   python main.py exemplo.ass -o exemplo_pt.ass")

    print("\n3️⃣  Traduzir para outro idioma:")
    print("   python main.py exemplo.ass -l en-US")

    print("\n4️⃣  Ajustar tamanho do batch:")
    print("   python main.py exemplo.ass -b 30")

    print("\n5️⃣  Ver todas as opções:")
    print("   python main.py --help")

    print("\n🌍 IDIOMAS SUPORTADOS:")
    languages = [
        "pt-BR (Português Brasileiro)",
        "en-US (Inglês)",
        "es-ES (Espanhol)",
        "fr-FR (Francês)",
        "de-DE (Alemão)",
        "it-IT (Italiano)",
        "ja-JP (Japonês)",
        "ko-KR (Coreano)",
    ]

    for lang in languages:
        print(f"   • {lang}")


def main():
    """Função principal do script de configuração."""
    print("🎬 TRADUTOR DE LEGENDAS ASS")
    print("=" * 40)

    # Configura ambiente
    if not setup_environment():
        return 1

    # Mostra exemplos de uso
    show_usage_examples()

    print(f"\n🚀 PRÓXIMOS PASSOS:")
    print("1. Configure sua GEMINI_API_KEY no arquivo .env")
    print("2. Execute o tradutor:")
    print(f"   python main.py sub.ass -o traslated.ass")

    return 0


if __name__ == "__main__":
    sys.exit(main())
